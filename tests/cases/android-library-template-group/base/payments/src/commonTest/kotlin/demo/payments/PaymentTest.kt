package demo.payments

import kotlin.test.Test
import kotlin.test.assertEquals

class PaymentTest {
    @Test
    fun labelNamesTheId() {
        assertEquals("payment 7", Payment("7", 100).label())
    }
}
