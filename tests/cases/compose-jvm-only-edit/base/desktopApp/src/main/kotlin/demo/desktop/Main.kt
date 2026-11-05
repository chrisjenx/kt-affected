package demo.desktop

import demo.ui.Greeting
import demo.ui.windowTitle

fun title(): String = windowTitle()

fun content() {
    Greeting("desktop")
}
