from copy import deepcopy

import pytest
from support.context import using_context
from tests.classic.perform_actions.support.refine import wait_for_events
from tests.support.helpers import center_point

pytestmark = pytest.mark.asyncio


@pytest.mark.parametrize("full_zoom", [0.5, 1.0, 2.0])
@pytest.mark.parametrize("devPixelsPerPx", [1.0, 2.0])
async def test_position_with_different_scaling(
    configuration, url, geckodriver, devPixelsPerPx, full_zoom
):
    config = deepcopy(configuration)

    prefs = config["capabilities"]["moz:firefoxOptions"].get("prefs", {})
    prefs.update({"layout.css.devPixelsPerPx": str(devPixelsPerPx)})
    config["capabilities"]["moz:firefoxOptions"]["prefs"] = prefs

    try:
        driver = geckodriver(config=config, extra_args=["--allow-system-access"])
        driver.new_session()

        driver.session.url = url("/webdriver/tests/support/html/test_actions.html")

        with using_context(driver.session, "chrome"):
            driver.session.execute_script(
                """
                const [full_zoom] = arguments;

                const { TabManager } = ChromeUtils.importESModule(
                    "chrome://remote/content/shared/TabManager.sys.mjs"
                );

                const tabBrowser = TabManager.getTabBrowser(window);
                tabBrowser.selectedBrowser.browsingContext.fullZoom = full_zoom;
            """,
                args=[full_zoom],
            )

        target = driver.session.find.css("#outer", all=False)
        expected_position = center_point(target)

        mouse_chain = driver.session.actions.sequence(
            "pointer", "pointer_id", {"pointerType": "mouse"}
        )
        mouse_chain.pointer_move(0, 0, origin=target).perform()

        events = wait_for_events(driver.session, 1)
        assert len(events) == 1

        assert events[0]["type"] == "mousemove"
        assert events[0]["pageX"] == pytest.approx(expected_position[0], abs=1.0)
        assert events[0]["pageY"] == pytest.approx(expected_position[1], abs=1.0)
        assert events[0]["target"] == "outer"
    finally:
        await driver.stop()
