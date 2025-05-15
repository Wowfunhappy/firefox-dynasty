/* -*- Mode: C++; tab-width: 2; indent-tabs-mode: nil; c-basic-offset: 2 -*- */
/* vim:set ts=2 sw=2 sts=2 et cindent: */
/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#include <dlfcn.h>

#include "AppleCVLinker.h"
#include "MainThreadUtils.h"
#include "nsDebug.h"

#ifdef PR_LOGGING
PRLogModuleInfo* GetAppleMediaLog();
#define LOG(...) PR_LOG(GetAppleMediaLog(), PR_LOG_DEBUG, (__VA_ARGS__))
#else
#define LOG(...)
#endif

CFStringRef AppleCVLinker::ColorPrimaries_P22 = nullptr;
CFStringRef AppleCVLinker::ColorPrimaries_P3_D65 = nullptr;
CFStringRef AppleCVLinker::ColorPrimaries_SMPTE_C = nullptr;
CFStringRef AppleCVLinker::ColorPrimaries_ITU_R_709_2 = nullptr;
CFStringRef AppleCVLinker::ColorPrimaries_ITU_R_2020 = nullptr;
CFStringRef AppleCVLinker::YCbCrMatrix_ITU_R_601_4 = nullptr;
CFStringRef AppleCVLinker::YCbCrMatrix_ITU_R_709_2 = nullptr;
CFStringRef AppleCVLinker::YCbCrMatrix_ITU_R_2020 = nullptr;
CFStringRef AppleCVLinker::TransferFunction_ITU_R_709_2 = nullptr;
CFStringRef AppleCVLinker::TransferFunction_sRGB = nullptr;
CFStringRef AppleCVLinker::TransferFunction_SMPTE_ST_2084_PQ = nullptr;
CFStringRef AppleCVLinker::TransferFunction_ITU_R_2100_HLG = nullptr;

namespace mozilla {

AppleCVLinker::LinkStatus
AppleCVLinker::sLinkStatus = LinkStatus_INIT;

void* AppleCVLinker::sLink = nullptr;
nsrefcnt AppleCVLinker::sRefCount = 0;

/* static */ bool
AppleCVLinker::Link()
{
  // Bump our reference count every time we're called.
  // Add a lock or change the thread assertion if
  // you need to call this off the main thread.
  MOZ_ASSERT(NS_IsMainThread());
  ++sRefCount;

  if (sLinkStatus) {
    return sLinkStatus == LinkStatus_SUCCEEDED;
  }

  const char* dlname =
    "/System/Library/Frameworks/CoreVideo.framework/CoreVideo";
  if (!(sLink = dlopen(dlname, RTLD_NOW | RTLD_LOCAL))) {
    NS_WARNING("Couldn't load CoreVideo framework");
    goto fail;
  }
  
  ColorPrimaries_P22 =
    GetIOConst("kCVImageBufferColorPrimaries_P22");
  ColorPrimaries_P3_D65 = 
    GetIOConst("kCVImageBufferColorPrimaries_P3_D65");
  ColorPrimaries_SMPTE_C = 
    GetIOConst("kCVImageBufferColorPrimaries_SMPTE_C");
  ColorPrimaries_ITU_R_709_2 = 
    GetIOConst("kCVImageBufferColorPrimaries_ITU_R_709_2");
  ColorPrimaries_ITU_R_2020 = 
    GetIOConst("kCVImageBufferColorPrimaries_ITU_R_2020");
  YCbCrMatrix_ITU_R_601_4 =
    GetIOConst("kCVImageBufferYCbCrMatrix_ITU_R_601_4");
  YCbCrMatrix_ITU_R_709_2 = 
    GetIOConst("kCVImageBufferYCbCrMatrix_ITU_R_709_2");
  YCbCrMatrix_ITU_R_2020 = 
    GetIOConst("kCVImageBufferYCbCrMatrix_ITU_R_2020");
  TransferFunction_ITU_R_709_2 = 
    GetIOConst("kCVImageBufferTransferFunction_ITU_R_709_2");
  TransferFunction_sRGB = 
    GetIOConst("kCVImageBufferTransferFunction_sRGB");
  TransferFunction_SMPTE_ST_2084_PQ = 
    GetIOConst("kCVImageBufferTransferFunction_SMPTE_ST_2084_PQ");
  TransferFunction_ITU_R_2100_HLG = 
    GetIOConst("kCVImageBufferTransferFunction_ITU_R_2100_HLG");
  
  LOG("Loaded CoreVideo framework.");
  sLinkStatus = LinkStatus_SUCCEEDED;
  return true;

fail:
  Unlink();

  sLinkStatus = LinkStatus_FAILED;
  return false;
}

/* static */ void
AppleCVLinker::Unlink()
{
  MOZ_ASSERT(NS_IsMainThread());
  MOZ_ASSERT(sRefCount > 0, "Unbalanced Unlink()");
  --sRefCount;
  if (sLink && sRefCount < 1) {
    LOG("Unlinking CoreVideo framework.");
    dlclose(sLink);
    sLink = nullptr;
  }
}

} // namespace mozilla
