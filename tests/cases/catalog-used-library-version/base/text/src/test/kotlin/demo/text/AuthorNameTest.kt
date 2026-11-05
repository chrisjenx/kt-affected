package demo.text

import kotlin.test.Test
import kotlin.test.assertEquals

class AuthorNameTest {
    @Test
    fun keepsName() {
        assertEquals("Ann", Author("Ann").name)
    }
}
