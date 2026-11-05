package p

class FakeListView : Component(), ListView {
    init {
        events += "fake"
    }

    val extra = events.size

    override fun title(): String = "Fake"
}
