package demo.ui

import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.runComposeUiTest
import kotlin.test.Test

@OptIn(ExperimentalTestApi::class)
class CounterTest {
    @Test
    fun showsTheCount() = runComposeUiTest {
        setContent { Counter(3) }
        onNodeWithText("Count: 3").assertExists()
    }
}
