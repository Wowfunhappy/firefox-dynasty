/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

/**
 * This function is a custom function in the style of the standard Intl.*
 * functions, that isn't part of any spec or proposal yet.
 *
 * Returns an object with the following properties:
 *   locale:
 *     The actual resolved locale.
 *
 *   calendar:
 *     The default calendar of the resolved locale.
 *
 *   firstDayOfWeek:
 *     The first day of the week for the resolved locale.
 *
 *   minDays:
 *     The minimum number of days in a week for the resolved locale.
 *
 *   weekend:
 *     The days of the week considered as the weekend for the resolved locale.
 *
 * Days are encoded as integers in the range 1=Monday to 7=Sunday.
 */
function Intl_getCalendarInfo(locales) {
  // 1. Let requestLocales be ? CanonicalizeLocaleList(locales).
  var requestedLocales = CanonicalizeLocaleList(locales);

  // 2. Let localeOpt be a new Record.
  var localeOpt = NEW_RECORD();

  // 3. Set localeOpt.[[localeMatcher]] to "best fit".
  localeOpt.localeMatcher = "best fit";

  // 4. Let r be ResolveLocale(%DateTimeFormat%.[[availableLocales]],
  //    requestedLocales, localeOpt).
  var r = intl_ResolveLocale(
    "DateTimeFormat",
    requestedLocales,
    localeOpt,
  );

  // 5. Let result be GetCalendarInfo(r.[[locale]]).
  var result = intl_GetCalendarInfo(r.locale);
  DefineDataProperty(result, "calendar", r.ca);
  DefineDataProperty(result, "locale", r.locale);

  // 6. Return result.
  return result;
}
