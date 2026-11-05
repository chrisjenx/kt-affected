package demo.billing

import demo.model.Money
import kotlin.test.Test

class BillingTest {
    @Test
    fun runs() {
        Money(100).format()
    }
}
