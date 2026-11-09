package demo.shelves

import kotlin.test.Test

abstract class ScreenWalker {
    @Test
    fun walks() {
        check(screens().isNotEmpty())
    }

    abstract fun screens(): List<String>
}

class ShelvesWalkerTest : ScreenWalker() {
    override fun screens() = listOf("shelves")
}
