package p

import kotlin.test.Test
import kotlin.test.assertEquals

class DetailComponentTest {
    @Test
    fun views() = assertEquals("Detail", DefaultDetailView().view())
}
