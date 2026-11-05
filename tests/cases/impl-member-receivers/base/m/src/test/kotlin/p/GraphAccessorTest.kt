package p

import kotlin.test.BeforeTest
import kotlin.test.Test

class GraphAccessorTest {
    private val r: RepoApi = Graph.repo

    @Test
    fun t() {
        r.foo()
    }
}
