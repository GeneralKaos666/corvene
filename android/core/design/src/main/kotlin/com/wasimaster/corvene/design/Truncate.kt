package com.wasimaster.corvene.design

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.LocalTextStyle
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp

/**
 * One line of [text], shortened in the middle when it does not fit (paths
 * and branch names keep both ends), or at the end with [middle] false. The
 * whole text stays in the semantics for screen readers.
 */
@Composable
fun Truncate(
    text: String,
    modifier: Modifier = Modifier,
    style: TextStyle = LocalTextStyle.current,
    color: Color = CorveneTheme.colors.textPrimary,
    middle: Boolean = true,
) {
    Text(
        text,
        modifier = modifier,
        style = style,
        color = color,
        maxLines = 1,
        softWrap = false,
        overflow = if (middle) TextOverflow.MiddleEllipsis else TextOverflow.Ellipsis,
    )
}

/**
 * A file path the way GitHub Desktop lists it: the directory dimmed, the
 * file name in the text colour (semibold in the GitHub styles' lists),
 * shortened in the middle. [oldPath] adds "old → " for renames.
 */
@Composable
fun FilePath(
    path: String,
    modifier: Modifier = Modifier,
    oldPath: String? = null,
    style: TextStyle = LocalTextStyle.current,
    boldName: Boolean = true,
) {
    val colors = CorveneTheme.colors
    val text = remember(path, oldPath, colors, boldName) {
        val slash = path.lastIndexOf('/')
        buildAnnotatedString {
            if (oldPath != null) {
                withStyle(SpanStyle(color = colors.textSecondary)) {
                    append(oldPath)
                    append(" → ")
                }
            }
            withStyle(SpanStyle(color = colors.textSecondary)) { append(path.substring(0, slash + 1)) }
            withStyle(SpanStyle(color = colors.textPrimary, fontWeight = if (boldName) FontWeight.SemiBold else null)) {
                append(path.substring(slash + 1))
            }
        }
    }
    Text(text, modifier = modifier, style = style, maxLines = 1, softWrap = false, overflow = TextOverflow.MiddleEllipsis)
}

@Preview(widthDp = 220, heightDp = 500)
@Composable
private fun TruncatePreview() {
    DesignStyleSamples {
        Column {
            Truncate("crates/corvene-ffi/src/vm/diff.rs")
            FilePath("android/feature/changes/src/main/kotlin/ChangesScreen.kt")
            FilePath("README.md", oldPath = "readme.txt", modifier = Modifier.padding(top = 4.dp))
        }
    }
}

