package demo

import kotlin.test.Test

class DirectTest {
    @Test
    fun direct() {
        val h = OrderPlacedHandler(Outbox())
        h.handle(Samples.first())
    }
}
