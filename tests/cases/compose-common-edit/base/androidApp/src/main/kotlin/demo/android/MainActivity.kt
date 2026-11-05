package demo.android

import demo.ui.Counter

class MainActivity {
    fun onCreate() {
        setContent { Counter(0) }
    }

    private fun setContent(content: () -> Unit) = content()
}
