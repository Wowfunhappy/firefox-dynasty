/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const MOCK_SITE_NAME = "example.com";

/**
 * Tests the site exclusion toggle visibility with VPN on or off.
 */
add_task(async function test_site_exclusion_toggle_with_siteData() {
  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: false,
    siteData: {
      isExclusion: false,
    },
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );

  // VPN is off
  Assert.ok(
    !content.siteExclusionControlEl,
    "Site exclusion control should not be present with VPN off"
  );

  let siteExclusionVisiblePromise = BrowserTestUtils.waitForMutationCondition(
    content.shadowRoot,
    { childList: true, subtree: true },
    () => content.siteExclusionControlEl
  );

  // Turn VPN on
  await setPanelState({
    isSignedOut: false,
    isProtectionEnabled: true,
    siteData: {
      isExclusion: false,
    },
  });

  await Promise.all([content.updateComplete, siteExclusionVisiblePromise]);

  Assert.ok(
    content.siteExclusionControlEl,
    "Site exclusion control should be present with VPN on"
  );
  Assert.ok(
    content.siteExclusionToggleEl,
    "Site exclusion toggle should be present with VPN on"
  );

  await closePanel();
});

/**
 * Tests that we don't show the site exclusion toggle if siteData is invalid.
 */
add_task(async function test_site_exclusion_toggle_no_siteData() {
  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: false,
    siteData: null,
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );
  Assert.ok(
    !content.siteExclusionControlEl,
    "Site exclusion control should not be present"
  );

  await closePanel();
});

/**
 * Tests that we don't show the site exclusion toggle when an error occurs.
 */
add_task(async function test_site_exclusion_VPN_error() {
  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: true,
    siteData: {
      isExclusion: false,
    },
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );
  Assert.ok(
    content.siteExclusionControlEl,
    "Site exclusion control should be present with VPN on"
  );

  let siteExclusionHiddenPromise = BrowserTestUtils.waitForMutationCondition(
    content.shadowRoot,
    { childList: true, subtree: true },
    () => !content.siteExclusionControlEl
  );

  // Turn VPN on
  await setPanelState({
    isSignedOut: false,
    isProtectionEnabled: true,
    siteData: {
      isExclusion: false,
    },
    error: "generic-error",
  });

  await Promise.all([content.updateComplete, siteExclusionHiddenPromise]);

  Assert.ok(
    !content.siteExclusionControlEl,
    "Site exclusion control should be not present due to an error"
  );

  await closePanel();
});

/**
 * Tests the site exclusion toggle is pressed if isExclusion is true.
 */
add_task(async function test_site_exclusion_toggle_pressed_isExclusion() {
  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: true,
    siteData: {
      isExclusion: false,
    },
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );
  Assert.ok(
    content.siteExclusionControlEl,
    "Site exclusion control should be present with VPN on"
  );
  Assert.ok(
    content.siteExclusionToggleEl,
    "Site exclusion toggle should be present with VPN on"
  );
  Assert.ok(
    !content.siteExclusionToggleEl.pressed,
    "Site exclusion toggle should not be in pressed state"
  );

  let togglePressedPromise = BrowserTestUtils.waitForMutationCondition(
    content.shadowRoot,
    {
      childList: true,
      subtree: true,
      attributes: true,
      attributeFilter: ["pressed"],
    },
    () => content.siteExclusionToggleEl?.pressed
  );

  // Set isExclusion to true
  await setPanelState({
    isSignedOut: false,
    isProtectionEnabled: true,
    siteData: {
      isExclusion: true,
    },
  });

  await Promise.all([content.updateComplete, togglePressedPromise]);

  Assert.ok(
    content.siteExclusionToggleEl?.pressed,
    "Site exclusion toggle should now be in pressed state"
  );

  await closePanel();
});
