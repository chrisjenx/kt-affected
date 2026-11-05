package demo.checkout

import demo.catalog.Catalog
import kotlin.test.Test
import kotlin.test.assertEquals

class CheckoutTest {
    @Test
    fun totals() {
        assertEquals(6, Checkout(Catalog()).total(listOf("12", "1234")))
    }
}
