package demo

import kotlin.test.Test

class BillingTest {
    @Test
    fun runs() {
        Billing().total(listOf(1, 2))
    }
}
