package p

import kotlin.test.Test
import kotlin.test.assertEquals

class FakeListViewTest {
    @Test
    fun records() = assertEquals(listOf("created", "fake"), FakeListView().events)
}
