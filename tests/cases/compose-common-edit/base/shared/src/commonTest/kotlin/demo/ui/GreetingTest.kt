package demo.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test

@OptIn(ExperimentalTestApi::class)
class GreetingTest {
    @Test
    fun showsTheName() = runComposeUiTest {
        setContent { Greeting("Ada") }
        onNodeWithText("Hello, Ada").assertExists()
    }
}
