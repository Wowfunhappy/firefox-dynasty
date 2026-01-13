/* -*- Mode: C++; tab-width: 8; indent-tabs-mode: nil; c-basic-offset: 2 -*-
 * vim: set ts=8 sts=2 et sw=2 tw=80:
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

#ifndef builtin_intl_LocaleNegotiation_h
#define builtin_intl_LocaleNegotiation_h

#include "js/RootingAPI.h"
#include "js/TypeDecls.h"

class JSLinearString;

namespace js {
class ArrayObject;
}

namespace js::intl {

enum class AvailableLocaleKind;

using LocalesList = JS::StackGCVector<JSLinearString*>;

/**
 * Canonicalizes a locale list.
 *
 * Spec: ECMAScript Internationalization API Specification, 9.2.1.
 */
bool CanonicalizeLocaleList(JSContext* cx, JS::Handle<JS::Value> locales,
                            JS::MutableHandle<LocalesList> result);

ArrayObject* LocalesListToArray(JSContext* cx, JS::Handle<LocalesList> locales);

/**
 * Compares a BCP 47 language tag against the locales in availableLocales and
 * returns the best available match -- or |nullptr| if no match was found.
 * Uses the fallback mechanism of RFC 4647, section 3.4.
 *
 * The set of available locales consulted doesn't necessarily include the
 * default locale or any generalized forms of it (e.g. "de" is a more-general
 * form of "de-CH"). If you want to be sure to consider the default local and
 * its generalized forms (you usually will), pass the default locale as the
 * value of |defaultLocale|; otherwise pass |nullptr|.
 *
 * Spec: ECMAScript Internationalization API Specification, 9.2.2.
 * Spec: RFC 4647, section 3.4.
 */
bool BestAvailableLocale(JSContext* cx, AvailableLocaleKind availableLocales,
                         JS::Handle<JSLinearString*> locale,
                         JS::Handle<JSLinearString*> defaultLocale,
                         JS::MutableHandle<JSLinearString*> result);

class LookupMatcherResult final {
  JSLinearString* locale_ = nullptr;
  JSLinearString* extension_ = nullptr;

 public:
  LookupMatcherResult() = default;
  LookupMatcherResult(JSLinearString* locale, JSLinearString* extension)
      : locale_(locale), extension_(extension) {}

  auto* locale() const { return locale_; }
  auto* extension() const { return extension_; }

  // Helper methods for WrappedPtrOperations.
  auto localeDoNotUse() const { return &locale_; }
  auto extensionDoNotUse() const { return &extension_; }

  // Trace implementation.
  void trace(JSTracer* trc);
};

/**
 * Compares a BCP 47 language priority list against the set of locales in
 * availableLocales and determines the best available language to meet the
 * request. Options specified through Unicode extension subsequences are
 * ignored in the lookup, but information about such subsequences is returned
 * separately.
 *
 * This variant is based on the Lookup algorithm of RFC 4647 section 3.4.
 *
 * Spec: ECMAScript Internationalization API Specification, 9.2.3.
 * Spec: RFC 4647, section 3.4.
 */
bool LookupMatcher(JSContext* cx, AvailableLocaleKind availableLocales,
                   JS::Handle<ArrayObject*> locales,
                   JS::MutableHandle<LookupMatcherResult> result);

/**
 * Return the supported locales in |locales| which are supported according to
 * |availableLocales|.
 */
ArrayObject* SupportedLocalesOf(JSContext* cx,
                                AvailableLocaleKind availableLocales,
                                JS::Handle<JS::Value> locales,
                                JS::Handle<JS::Value> options);

/**
 * Return the supported locale for the default locale if ICU supports that
 * default locale (perhaps via fallback, e.g. supporting "de-CH" through "de"
 * support implied by a "de-DE" locale). Otherwise uses the last-ditch locale.
 */
JSLinearString* ComputeDefaultLocale(JSContext* cx);

}  // namespace js::intl

namespace js {

template <typename Wrapper>
class WrappedPtrOperations<intl::LookupMatcherResult, Wrapper> {
  const auto& container() const {
    return static_cast<const Wrapper*>(this)->get();
  }

 public:
  JS::Handle<JSLinearString*> locale() const {
    return JS::Handle<JSLinearString*>::fromMarkedLocation(
        container().localeDoNotUse());
  }

  JS::Handle<JSLinearString*> extension() const {
    return JS::Handle<JSLinearString*>::fromMarkedLocation(
        container().extensionDoNotUse());
  }
};

}  // namespace js

#endif /* builtin_intl_LocaleNegotiation_h */
