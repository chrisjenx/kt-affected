package demo.app

import kotlin.test.Test
import kotlin.test.assertTrue

class LogoTest {
    @Test
    fun logo() {
        assertTrue(Art.logo() != 0)
    }
}
