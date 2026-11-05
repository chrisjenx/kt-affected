package p

import kotlin.test.BeforeTest
import kotlin.test.Test

class BarOnlyTest {


    @Test
    fun t() {
        RepoImpl().bar()
    }
}
