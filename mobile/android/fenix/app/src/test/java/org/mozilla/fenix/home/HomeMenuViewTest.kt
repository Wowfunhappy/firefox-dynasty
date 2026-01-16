/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package org.mozilla.fenix.home

import android.view.View
import androidx.lifecycle.LifecycleOwner
import androidx.navigation.NavController
import androidx.navigation.NavDirections
import androidx.navigation.NavOptions
import io.mockk.every
import io.mockk.just
import io.mockk.mockk
import io.mockk.runs
import io.mockk.spyk
import io.mockk.verify
import mozilla.components.browser.menu.view.MenuButton
import mozilla.components.support.test.robolectric.testContext
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import org.mozilla.fenix.R
import org.mozilla.fenix.helpers.FenixGleanTestRule
import org.robolectric.RobolectricTestRunner
import java.lang.ref.WeakReference

@RunWith(RobolectricTestRunner::class)
class HomeMenuViewTest {

    @get:Rule
    val gleanTestRule = FenixGleanTestRule(testContext)

    private lateinit var view: View
    private lateinit var lifecycleOwner: LifecycleOwner
    private lateinit var navController: NavController
    private lateinit var menuButton: MenuButton
    private lateinit var homeMenuView: HomeMenuView

    @Before
    fun setup() {
        view = mockk(relaxed = true)
        lifecycleOwner = mockk(relaxed = true)
        navController = mockk(relaxed = true) {
            every { navigate(any<NavDirections>(), any<NavOptions>()) } just runs
            every { currentDestination?.id } returns R.id.homeFragment
        }

        menuButton = spyk(MenuButton(testContext))

        homeMenuView = HomeMenuView(
            context = testContext,
            navController = navController,
            menuButton = WeakReference(menuButton),
        )
    }

    @Test
    fun `WHEN dismiss menu is called THEN the menu is dismissed`() {
        homeMenuView.dismissMenu()

        verify {
            menuButton.dismissMenu()
        }
    }
}
