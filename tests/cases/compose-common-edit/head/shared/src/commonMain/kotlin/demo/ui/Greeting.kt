package demo.ui

import androidx.compose.material3.Text
import androidx.compose.runtime.Composable

@Composable
fun Greeting(name: String) {
    Text(greetingText(name).uppercase())
}

fun greetingText(name: String): String = "Hello, $name"
