package demo

import kotlin.test.Test
import kotlin.test.assertTrue

class SearchIndexTest {
    private val index = SearchIndex()

    @Test
    fun finds() {
        index.add("hay")
        assertTrue(index.has("hay"))
    }
}
