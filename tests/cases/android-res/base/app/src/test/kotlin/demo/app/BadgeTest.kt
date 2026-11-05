package demo.app

import kotlin.test.Test
import kotlin.test.assertTrue

class BadgeTest {
    @Test
    fun badge() {
        assertTrue(Art.badge() != 0)
    }
}
