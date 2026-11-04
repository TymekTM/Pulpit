//! The glyph on a multi-cell key grows with the key. The table here is
//! the same one apps/desktop/tests/catalog.test.js checks against
//! catalog.js tileIconScale, so both surfaces draw the same glyph size.

package app.pulpit.mobile.ui

import org.junit.Assert.assertEquals
import org.junit.Test

class TileIconScaleTest {
    @Test
    fun growsAQuarterPerExtraCellUpToTheCap() {
        assertEquals(1f, tileIconScale(1, 1), 1e-6f)
        assertEquals(1.25f, tileIconScale(2, 1), 1e-6f)
        assertEquals(1.25f, tileIconScale(1, 2), 1e-6f)
        assertEquals(1.5f, tileIconScale(2, 2), 1e-6f)
        assertEquals(1.6f, tileIconScale(3, 2), 1e-6f)
        assertEquals(1.6f, tileIconScale(8, 8), 1e-6f)
    }

    @Test
    fun degenerateSizesActLikeOneCell() {
        assertEquals(1f, tileIconScale(0, 0), 1e-6f)
        assertEquals(1f, tileIconScale(-3, 1), 1e-6f)
    }
}
