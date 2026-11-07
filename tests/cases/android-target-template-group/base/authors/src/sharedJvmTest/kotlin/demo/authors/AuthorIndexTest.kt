package demo.authors

import kotlin.test.Test
import kotlin.test.assertEquals

class AuthorIndexTest {
    @Test
    fun countsEveryAdd() {
        val shelf = AuthorIndex()
        shelf.add("Ada")
        shelf.add("Grace")
        assertEquals(2, shelf.count())
    }
}
