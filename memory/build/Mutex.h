/* -*- Mode: C++; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* vim: set ts=8 sts=2 et sw=2 tw=80: */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef Mutex_h
#define Mutex_h

#if defined(XP_WIN)
#  include <windows.h>
#else
#  include <pthread.h>
#endif
#if defined(XP_DARWIN)
#  include <os/lock.h>
#  include <libkern/OSAtomic.h>
#endif

#include "mozilla/Assertions.h"
#include "mozilla/Attributes.h"
#include "mozilla/ThreadSafety.h"

#if defined(XP_DARWIN)
// For information about the following undocumented flags and functions see
// https://github.com/apple/darwin-xnu/blob/main/bsd/sys/ulock.h and
// https://github.com/apple/darwin-libplatform/blob/main/private/os/lock_private.h
#  define OS_UNFAIR_LOCK_DATA_SYNCHRONIZATION (0x00010000)
#  define OS_UNFAIR_LOCK_ADAPTIVE_SPIN (0x00040000)

extern "C" {

typedef uint32_t os_unfair_lock_options_t;
OS_UNFAIR_LOCK_AVAILABILITY
OS_EXPORT OS_NOTHROW OS_NONNULL_ALL void os_unfair_lock_lock_with_options(
    os_unfair_lock_t lock, os_unfair_lock_options_t options);
}
static_assert(OS_UNFAIR_LOCK_INIT._os_unfair_lock_opaque == OS_SPINLOCK_INIT,
              "OS_UNFAIR_LOCK_INIT and OS_SPINLOCK_INIT have the same "
              "value");
static_assert(sizeof(os_unfair_lock) == sizeof(OSSpinLock),
              "os_unfair_lock and OSSpinLock are the same size");
#endif

// Mutexes based on spinlocks.  We can't use normal pthread spinlocks in all
// places, because they require malloc()ed memory, which causes bootstrapping
// issues in some cases.  We also can't use constructors, because for statics,
// they would fire after the first use of malloc, resetting the locks.
struct MOZ_CAPABILITY("mutex") Mutex {
#if defined(XP_WIN)
  CRITICAL_SECTION mMutex;
#elif defined(XP_DARWIN)
  union {
    os_unfair_lock mUnfairLock;
    OSSpinLock mSpinLock;
  } mMutex;
#else
  pthread_mutex_t mMutex;
#endif
  // Initializes a mutex. Returns whether initialization succeeded.
  inline bool Init() {
#if defined(XP_WIN)
    if (!InitializeCriticalSectionAndSpinCount(&mMutex, 5000)) {
      return false;
    }
#elif defined(XP_DARWIN)
    // The hack below works because both OS_UNFAIR_LOCK_INIT and
    // OS_SPINLOCK_INIT initialize the lock to 0 and in both case it's a 32-bit
    // integer.
    mMutex.mSpinLock = OS_SPINLOCK_INIT;
#elif defined(XP_LINUX) && !defined(ANDROID)
    pthread_mutexattr_t attr;
    if (pthread_mutexattr_init(&attr) != 0) {
      return false;
    }
    pthread_mutexattr_settype(&attr, PTHREAD_MUTEX_ADAPTIVE_NP);
    if (pthread_mutex_init(&mMutex, &attr) != 0) {
      pthread_mutexattr_destroy(&attr);
      return false;
    }
    pthread_mutexattr_destroy(&attr);
#else
    if (pthread_mutex_init(&mMutex, nullptr) != 0) {
      return false;
    }
#endif
    return true;
  }

  inline void Lock() MOZ_CAPABILITY_ACQUIRE() {
#if defined(XP_WIN)
    EnterCriticalSection(&mMutex);
#elif defined(XP_DARWIN)
    // We rely on a non-public function to improve performance here.
    // The OS_UNFAIR_LOCK_DATA_SYNCHRONIZATION flag informs the kernel that
    // the calling thread is able to make progress even in absence of actions
    // from other threads and the OS_UNFAIR_LOCK_ADAPTIVE_SPIN one causes the
    // kernel to spin on a contested lock if the owning thread is running on
    // the same physical core (presumably only on x86 CPUs given that ARM
    // macs don't have cores capable of SMT).
      if (!Mutex::gSpinInKernelSpace) {
       OSSpinLockLock(&mMutex.mSpinLock);
      } else {
#  if defined(__x86_64__)
        if(__builtin_available(macOS 10.15, *)) {
          os_unfair_lock_lock_with_options(&mMutex.mUnfairLock,
                                         OS_UNFAIR_LOCK_ADAPTIVE_SPIN | OS_UNFAIR_LOCK_DATA_SYNCHRONIZATION);
        } else {
          // On older versions of macOS (10.14 and older) the
          // `OS_UNFAIR_LOCK_ADAPTIVE_SPIN` flag is not supported by the kernel,
          // we spin in user-space instead like `OSSpinLock` does:
          // https://github.com/apple/darwin-libplatform/blob/215b09856ab5765b7462a91be7076183076600df/src/os/lock.c#L183-L198
          // Note that `OSSpinLock` uses 1000 iterations on x86-64:
          // https://github.com/apple/darwin-libplatform/blob/215b09856ab5765b7462a91be7076183076600df/src/os/lock.c#L93
          // ...but we only use 100 like it does on ARM:
          // https://github.com/apple/darwin-libplatform/blob/215b09856ab5765b7462a91be7076183076600df/src/os/lock.c#L90
          // We choose this value because it yields the same results in our
          // benchmarks but is less likely to have detrimental effects caused by
          // excessive spinning.
          uint32_t retries = 100;

          do {
            if (os_unfair_lock_trylock(&mMutex.mUnfairLock)) {
              return;
            }
            __asm__ __volatile__("pause");
          } while (retries--);

          os_unfair_lock_lock_with_options(&mMutex.mUnfairLock,
                                         OS_UNFAIR_LOCK_DATA_SYNCHRONIZATION);
        }
#  else
      MOZ_CRASH("User-space spin-locks should never be used on ARM");
#  endif  // defined(__x86_64__)
     }
#else
    pthread_mutex_lock(&mMutex);
#endif
  }

  [[nodiscard]] bool TryLock() MOZ_TRY_ACQUIRE(true);

  inline void Unlock() MOZ_CAPABILITY_RELEASE() {
#if defined(XP_WIN)
    LeaveCriticalSection(&mMutex);
#elif defined(XP_DARWIN)
    if (!Mutex::gSpinInKernelSpace) {
      OSSpinLockUnlock(&mMutex.mSpinLock);
    } else {
      os_unfair_lock_unlock(&mMutex.mUnfairLock);
    }
#else
    pthread_mutex_unlock(&mMutex);
#endif
  }

#if defined(XP_DARWIN)
  static bool SpinInKernelSpace();
  static const bool gSpinInKernelSpace;
#endif  // XP_DARWIN
};

// Mutex that can be used for static initialization.
// On Windows, CRITICAL_SECTION requires a function call to be initialized,
// but for the initialization lock, a static initializer calling the
// function would be called too late. We need no-function-call
// initialization, which SRWLock provides.
// Ideally, we'd use the same type of locks everywhere, but SRWLocks
// everywhere incur a performance penalty. See bug 1418389.
#if defined(XP_WIN)
struct MOZ_CAPABILITY("mutex") StaticMutex {
  SRWLOCK mMutex;

  inline void Lock() MOZ_CAPABILITY_ACQUIRE() {
    AcquireSRWLockExclusive(&mMutex);
  }

  inline void Unlock() MOZ_CAPABILITY_RELEASE() {
    ReleaseSRWLockExclusive(&mMutex);
  }
};

// Normally, we'd use a constexpr constructor, but MSVC likes to create
// static initializers anyways.
#  define STATIC_MUTEX_INIT SRWLOCK_INIT

#else
typedef Mutex StaticMutex;

#  if defined(XP_DARWIN)
// The hack below works because both OS_UNFAIR_LOCK_INIT and OS_SPINLOCK_INIT
// initialize the lock to 0 and in both case it's a 32-bit integer.
#    define STATIC_MUTEX_INIT \
      { .mUnfairLock = OS_UNFAIR_LOCK_INIT }
#  elif defined(XP_LINUX) && !defined(ANDROID)
#    define STATIC_MUTEX_INIT PTHREAD_ADAPTIVE_MUTEX_INITIALIZER_NP
#  else
#    define STATIC_MUTEX_INIT PTHREAD_MUTEX_INITIALIZER
#  endif

#endif

#ifdef XP_WIN
typedef DWORD ThreadId;
inline ThreadId GetThreadId() { return GetCurrentThreadId(); }
inline bool ThreadIdEqual(ThreadId a, ThreadId b) { return a == b; }
#else
typedef pthread_t ThreadId;
inline ThreadId GetThreadId() { return pthread_self(); }
inline bool ThreadIdEqual(ThreadId a, ThreadId b) {
  return pthread_equal(a, b);
}
#endif

class MOZ_CAPABILITY("mutex") MaybeMutex : public Mutex {
 public:
  enum DoLock {
    MUST_LOCK,
    AVOID_LOCK_UNSAFE,
  };

  bool Init(DoLock aDoLock) {
    mDoLock = aDoLock;
#ifdef MOZ_DEBUG
    mThreadId = GetThreadId();
#endif
    return Mutex::Init();
  }

#ifndef XP_WIN
  // Re initialise after fork(), assumes that mDoLock is already initialised.
  void Reinit(pthread_t aForkingThread) {
    if (mDoLock == MUST_LOCK) {
      Mutex::Init();
      return;
    }
#  ifdef MOZ_DEBUG
    // If this is an eluded lock we can only safely re-initialise it if the
    // thread that called fork is the one that owns the lock.
    if (pthread_equal(mThreadId, aForkingThread)) {
      mThreadId = GetThreadId();
      Mutex::Init();
    } else {
      // We can't guantee that whatever resource this lock protects (probably a
      // jemalloc arena) is in a consistent state.
      mDeniedAfterFork = true;
    }
#  endif
  }
#endif

  inline void Lock() MOZ_CAPABILITY_ACQUIRE() {
    if (ShouldLock()) {
      Mutex::Lock();
    }
  }

  inline void Unlock() MOZ_CAPABILITY_RELEASE() {
    if (ShouldLock()) {
      Mutex::Unlock();
    }
  }

  // Return true if we can use this resource from this thread, either because
  // we'll use the lock or because this is the only thread that will access the
  // protected resource.
#ifdef MOZ_DEBUG
  bool SafeOnThisThread() const {
    return mDoLock == MUST_LOCK || ThreadIdEqual(GetThreadId(), mThreadId);
  }
#endif

  bool LockIsEnabled() const { return mDoLock == MUST_LOCK; }

 private:
  bool ShouldLock() {
#ifndef XP_WIN
    MOZ_ASSERT(!mDeniedAfterFork);
#endif

    if (mDoLock == MUST_LOCK) {
      return true;
    }

    MOZ_ASSERT(ThreadIdEqual(GetThreadId(), mThreadId));
    return false;
  }

  DoLock mDoLock;
#ifdef MOZ_DEBUG
  ThreadId mThreadId;
#  ifndef XP_WIN
  bool mDeniedAfterFork = false;
#  endif
#endif
};

template <typename T>
struct MOZ_SCOPED_CAPABILITY MOZ_RAII AutoLock {
  explicit AutoLock(T& aMutex) MOZ_CAPABILITY_ACQUIRE(aMutex) : mMutex(aMutex) {
    mMutex.Lock();
  }

  ~AutoLock() MOZ_CAPABILITY_RELEASE() { mMutex.Unlock(); }

  AutoLock(const AutoLock&) = delete;
  AutoLock(AutoLock&&) = delete;

 private:
  T& mMutex;
};

using MutexAutoLock = AutoLock<Mutex>;

using MaybeMutexAutoLock = AutoLock<MaybeMutex>;

#endif
