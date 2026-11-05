package p

class FakeListView : Component(), ListView {
    init {
        events += "fake"
    }

    override fun title(): String = "Fake"
}
