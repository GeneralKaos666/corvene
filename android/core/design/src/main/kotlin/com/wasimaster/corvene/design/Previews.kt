package com.wasimaster.corvene.design

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

/**
 * [content] once per design style, labelled, for a component's `@Preview`
 * (and the screenshot tests): every public composable here is seen in all
 * three skins at once.
 */
@Composable
fun DesignStyleSamples(
    modifier: Modifier = Modifier,
    colorMode: ColorMode = ColorMode.Light,
    content: @Composable () -> Unit,
) {
    Column(modifier) {
        DesignStyle.entries.forEach { style ->
            CorveneTheme(style = style, colorMode = colorMode, dynamicColor = false) {
                Column(
                    Modifier.background(CorveneTheme.colors.bgCanvas).padding(12.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Text(style.name, color = CorveneTheme.colors.textSecondary, style = CorveneTheme.textStyles.codeSmall)
                    content()
                }
            }
        }
    }
}
