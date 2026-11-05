package demo.legacy

import demo.catalog.Catalog
import kotlin.test.Test
import kotlin.test.assertEquals

class LegacyTest {
    @Test
    fun prices() {
        assertEquals(3, Legacy(Catalog()).price())
    }
}
