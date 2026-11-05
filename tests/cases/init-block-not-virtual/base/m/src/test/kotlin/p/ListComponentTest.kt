package p

import kotlin.test.Test
import kotlin.test.assertEquals

class ListComponentTest {
    @Test
    fun records() = assertEquals(listOf("created", "list"), DefaultListView().events)
}
