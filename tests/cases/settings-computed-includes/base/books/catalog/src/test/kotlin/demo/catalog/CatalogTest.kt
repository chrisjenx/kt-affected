package demo.catalog

import kotlin.test.Test
import kotlin.test.assertEquals

class CatalogTest {
    @Test
    fun prices() {
        assertEquals(4, Catalog().price("1234"))
    }
}
