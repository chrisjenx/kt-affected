package demo

import kotlin.test.Test
import kotlin.test.assertNotNull

class BannerTest {
    @Test
    fun hasABanner() {
        assertNotNull(bannerArt())
    }
}
