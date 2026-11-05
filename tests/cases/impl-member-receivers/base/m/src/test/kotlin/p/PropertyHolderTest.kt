package p

import kotlin.test.BeforeTest
import kotlin.test.Test

class PropertyHolderTest {
    private val r: RepoApi = RepoImpl()

    @Test
    fun t() {
        r.foo()
    }
}
