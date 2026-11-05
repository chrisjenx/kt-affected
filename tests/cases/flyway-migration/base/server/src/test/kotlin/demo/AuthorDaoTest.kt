package demo

import kotlin.test.Test

class AuthorDaoTest {
    @Test
    fun runs() {
        AuthorDao().count()
    }
}
