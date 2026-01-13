/* Any copyright is dedicated to the Public Domain.
 * http://creativecommons.org/publicdomain/zero/1.0/ */

"use strict";

const FIRSTRUN_URL = "chrome://browser/content/aiwindow/firstrun.html";

async function openFirstrunPage() {
  const tab = await BrowserTestUtils.openNewForegroundTab(
    gBrowser,
    FIRSTRUN_URL
  );
  return tab;
}

add_task(async function test_firstrun_welcome_screen_renders() {
  await SpecialPowers.pushPrefEnv({
    set: [["browser.aiwindow.firstrun.autoAdvanceMS", 0]],
  });

  const tab = await openFirstrunPage();

  await SpecialPowers.spawn(tab.linkedBrowser, [], async () => {
    await ContentTaskUtils.waitForCondition(
      () => content.document.querySelector(".screen.AI_WINDOW_INTRO"),
      "Wait for the AI Window intro screen to be rendered"
    );

    const introScreen = content.document.querySelector(
      ".screen.AI_WINDOW_INTRO"
    );
    Assert.ok(
      introScreen,
      "The intro screen with class 'screen AI_WINDOW_INTRO' should be present"
    );

    await ContentTaskUtils.waitForCondition(
      () => content.document.querySelector(".screen.AI_WINDOW_CHOOSE_MODEL"),
      "Wait for the AI Window choose model screen to be rendered"
    );
  });

  BrowserTestUtils.removeTab(tab);
  await SpecialPowers.popPrefEnv();
});

add_task(async function test_launchWindow_shows_firstrun_when_not_completed() {
  await SpecialPowers.pushPrefEnv({
    set: [
      ["browser.aiwindow.enabled", true],
      ["browser.aiwindow.firstrun.hasCompleted", false],
    ],
  });

  // Ensure we start in classic mode
  document.documentElement.removeAttribute("ai-window");

  const tab = await BrowserTestUtils.openNewForegroundTab(
    gBrowser,
    "about:blank"
  );

  await AIWindow.launchWindow(gBrowser.selectedBrowser);

  await BrowserTestUtils.waitForCondition(
    () => gBrowser.selectedBrowser.currentURI.spec === FIRSTRUN_URL,
    "Should navigate to firstrun.html"
  );

  Assert.equal(
    gBrowser.selectedBrowser.currentURI.spec,
    FIRSTRUN_URL,
    "launchWindow should load firstrun.html when firstrun not completed"
  );

  // Cleanup
  document.documentElement.removeAttribute("ai-window");
  BrowserTestUtils.removeTab(tab);
  await SpecialPowers.popPrefEnv();
});

add_task(async function test_switcher_shows_firstrun_when_not_completed() {
  await SpecialPowers.pushPrefEnv({
    set: [
      ["browser.aiwindow.enabled", true],
      ["browser.aiwindow.firstrun.hasCompleted", false],
    ],
  });

  // Ensure we start in classic mode
  document.documentElement.removeAttribute("ai-window");

  const tab = await BrowserTestUtils.openNewForegroundTab(
    gBrowser,
    "about:blank"
  );

  // Open the switcher panel and click "Switch to AI Window"
  let button = document.getElementById("ai-window-toggle");
  let view = PanelMultiView.getViewNode(document, "ai-window-toggle-view");

  let viewShownPromise = BrowserTestUtils.waitForEvent(view, "ViewShown");
  button.click();
  await viewShownPromise;

  let aiButton = view.querySelector("#ai-window-switch-ai");
  aiButton.click();

  await TestUtils.waitForCondition(
    () => document.documentElement.hasAttribute("ai-window"),
    "Window should have ai-window attribute after switching"
  );

  await BrowserTestUtils.waitForCondition(
    () => gBrowser.selectedBrowser.currentURI.spec === FIRSTRUN_URL,
    "Should navigate to firstrun.html"
  );

  Assert.equal(
    gBrowser.selectedBrowser.currentURI.spec,
    FIRSTRUN_URL,
    "Switcher should load firstrun.html when firstrun not completed"
  );

  await TestUtils.waitForCondition(
    () => PanelUI.panel.state === "closed",
    "Panel should close after switching"
  );

  // Cleanup
  document.documentElement.removeAttribute("ai-window");
  BrowserTestUtils.removeTab(tab);
  await SpecialPowers.popPrefEnv();
});
