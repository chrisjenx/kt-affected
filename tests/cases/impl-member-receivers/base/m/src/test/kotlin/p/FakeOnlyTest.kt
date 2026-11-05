package p

import kotlin.test.BeforeTest
import kotlin.test.Test

class FakeOnlyTest {
    private val r: RepoApi = object : RepoApi {
        override fun foo() = 9
        override fun bar() = 9
    }

    @Test
    fun t() {
        r.foo()
    }
}
