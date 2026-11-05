package p

import kotlin.test.BeforeTest
import kotlin.test.Test

class LateinitTest {
    private lateinit var r: RepoApi

    @BeforeTest
    fun setUp() {
        r = RepoImpl()
    }

    @Test
    fun t() {
        r.foo()
    }
}
