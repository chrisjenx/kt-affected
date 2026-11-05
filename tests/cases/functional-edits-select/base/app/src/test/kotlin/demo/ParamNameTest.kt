package demo

import kotlin.test.Test

class ParamNameTest {
    @Test
    fun runs() {
        ParamName().greet(name = "a")
    }
}
