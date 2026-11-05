package demo.billing

import kotlin.test.Test

class BillingTest {
    @Test
    fun runs() {
        demo.model.Money(100).format()
    }
}
