/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

"use strict";

const { IPPExceptionsManager } = ChromeUtils.importESModule(
  "moz-src:///browser/components/ipprotection/IPPExceptionsManager.sys.mjs"
);

const MOCK_SITE_NAME = "https://example.com";

const PERM_NAME = "ipp-vpn";

const TOGGLE_ON_EVENT = "IPProtection:ToggleOnExclusion";
const TOGGLE_OFF_EVENT = "IPProtection:ToggleOffExclusion";

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

/**
 * Tests the site exclusion toggle dispatches the expected events and calls
 * the appropriate IPPExceptionsManager methods.
 */
add_task(async function test_site_exclusion_toggle_events() {
  const sandbox = sinon.createSandbox();
  Services.perms.removeByType(PERM_NAME);

  let setExclusionSpy = sandbox.spy(IPPExceptionsManager, "setExclusion");

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

  // Toggle ON exclusion
  let toggleOnEventPromise = BrowserTestUtils.waitForEvent(
    window,
    TOGGLE_ON_EVENT
  );
  content.siteExclusionToggleEl.click();
  await toggleOnEventPromise;

  Assert.ok(true, "Toggle exclusion ON event was dispatched");
  Assert.ok(
    setExclusionSpy.calledOnce,
    "IPPExceptionsManager.setExclusion should be called after toggling exclusion to ON"
  );
  Assert.strictEqual(
    setExclusionSpy.firstCall.args[1],
    true,
    "IPPExceptionsManager.setExclusion should be called with shouldExclude=true"
  );

  // Toggle OFF exclusion
  let toggleOffEventPromise = BrowserTestUtils.waitForEvent(
    window,
    TOGGLE_OFF_EVENT
  );
  content.siteExclusionToggleEl.click();
  await toggleOffEventPromise;

  Assert.ok(true, "Toggle exclusion OFF event was dispatched");
  Assert.ok(
    setExclusionSpy.calledTwice,
    "IPPExceptionsManager.setExclusion should be called two times now"
  );
  Assert.strictEqual(
    setExclusionSpy.secondCall.args[1],
    false,
    "IPPExceptionsManager.setExclusion should be called with shouldExclude=false"
  );

  await closePanel();

  // Clean up
  Services.perms.removeByType(PERM_NAME);
  sandbox.restore();
});

/**
 * Tests that siteData and toggle pressed state update when navigating from
 * a non excluded site to an excluded site in a different tab.
 */
add_task(
  async function test_site_exclusion_updates_on_navigation_different_tab() {
    const sandbox = sinon.createSandbox();
    Services.perms.removeByType(PERM_NAME);

    const FIRST_SITE = "https://example.com";
    const SECOND_SITE = "https://example.org";

    // Add second site as an exclusion
    const secondSitePrincipal =
      Services.scriptSecurityManager.createContentPrincipalFromOrigin(
        SECOND_SITE
      );
    Services.perms.addFromPrincipal(
      secondSitePrincipal,
      PERM_NAME,
      Ci.nsIPermissionManager.DENY_ACTION
    );

    // Open first tab (not excluded)
    let tab1 = await BrowserTestUtils.openNewForegroundTab(
      gBrowser,
      FIRST_SITE
    );

    let content = await openPanel({
      isSignedOut: false,
      isProtectionEnabled: true,
    });

    Assert.ok(
      BrowserTestUtils.isVisible(content),
      "ipprotection content component should be present"
    );
    Assert.ok(
      content.siteExclusionToggleEl,
      "Site exclusion toggle should be present"
    );
    Assert.ok(
      !content.siteExclusionToggleEl.pressed,
      "Toggle should not be pressed for first site (not excluded)"
    );

    // Now open the second tab (site excluded)
    let tab2 = await BrowserTestUtils.openNewForegroundTab(
      gBrowser,
      SECOND_SITE
    );

    let siteDataUpdatePromise = BrowserTestUtils.waitForMutationCondition(
      content.shadowRoot,
      {
        childList: true,
        subtree: true,
        attributes: true,
      },
      () => content.state?.siteData
    );

    await Promise.all([content.updateComplete, siteDataUpdatePromise]);

    Assert.ok(
      content.siteExclusionToggleEl.pressed,
      "Toggle should be pressed for the second site (which is excluded)"
    );

    await closePanel();
    BrowserTestUtils.removeTab(tab2);
    BrowserTestUtils.removeTab(tab1);

    Services.perms.removeByType(PERM_NAME);
    sandbox.restore();
  }
);

/**
 * Tests that siteData and toggle pressed state update when navigating from
 * a non excluded site to an excluded site in the same tab.
 */
add_task(async function test_site_exclusion_updates_on_navigation_same_tab() {
  const sandbox = sinon.createSandbox();
  Services.perms.removeByType(PERM_NAME);

  const FIRST_SITE = "https://example.com";
  const SECOND_SITE = "https://example.org";

  const secondSitePrincipal =
    Services.scriptSecurityManager.createContentPrincipalFromOrigin(
      SECOND_SITE
    );
  Services.perms.addFromPrincipal(
    secondSitePrincipal,
    PERM_NAME,
    Ci.nsIPermissionManager.DENY_ACTION
  );

  // Open the first site (not excluded) first
  let tab = await BrowserTestUtils.openNewForegroundTab(gBrowser, FIRST_SITE);

  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: true,
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );
  Assert.ok(
    content.siteExclusionToggleEl,
    "Site exclusion toggle should be present"
  );
  Assert.ok(
    !content.siteExclusionToggleEl.pressed,
    "Toggle should not be pressed for first site (not excluded)"
  );

  let siteDataUpdatePromise = BrowserTestUtils.waitForMutationCondition(
    content.shadowRoot,
    {
      childList: true,
      subtree: true,
      attributes: true,
    },
    () => content.state?.siteData
  );

  // Now load the second site (excluded)
  BrowserTestUtils.startLoadingURIString(tab.linkedBrowser, SECOND_SITE);
  await BrowserTestUtils.browserLoaded(tab.linkedBrowser);

  await Promise.all([content.updateComplete, siteDataUpdatePromise]);

  Assert.ok(
    content.siteExclusionToggleEl.pressed,
    "Toggle should be pressed for the second site (which is excluded)"
  );

  await closePanel();
  BrowserTestUtils.removeTab(tab);

  Services.perms.removeByType(PERM_NAME);
  sandbox.restore();
});

/**
 * Tests that we don't show the site exclusion toggle on privileged pages.
 */
add_task(async function test_site_exclusion_toggle_privileged_page() {
  const sandbox = sinon.createSandbox();
  const ABOUT_PAGE = "about:about";

  let panel = IPProtection.getPanel(window);
  sandbox.stub(panel, "_isPrivilegedPage").returns(true);

  let tab = await BrowserTestUtils.openNewForegroundTab(gBrowser, ABOUT_PAGE);

  let content = await openPanel({
    isSignedOut: false,
    isProtectionEnabled: true,
  });

  Assert.ok(
    BrowserTestUtils.isVisible(content),
    "ipprotection content component should be present"
  );
  Assert.ok(
    !content.siteExclusionControlEl,
    "Site exclusion control should not be present on privileged pages"
  );

  await closePanel();
  BrowserTestUtils.removeTab(tab);
  sandbox.restore();
});
