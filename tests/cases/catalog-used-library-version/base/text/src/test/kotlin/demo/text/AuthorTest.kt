package demo.text

import kotlin.test.Test
import kotlin.test.assertEquals

class AuthorTest {
    @Test
    fun digestIsStable() {
        assertEquals(64, Author("Ann").digest().length)
    }
}
