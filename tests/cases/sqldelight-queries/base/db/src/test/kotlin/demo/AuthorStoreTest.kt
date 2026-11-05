package demo

import kotlin.test.Test

class AuthorStoreTest {
    @Test
    fun finds() {
        AuthorStore(testDb().authorQueries).named("Ada")
    }
}
