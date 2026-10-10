package __PACKAGE__

import android.content.Context
import androidx.startup.Initializer
import androidx.window.core.ExperimentalWindowApi
import androidx.window.embedding.RuleController

@OptIn(ExperimentalWindowApi::class)
class WebCaskSplitInitializer : Initializer<RuleController> {
    override fun create(context: Context): RuleController {
        return RuleController.getInstance(context).apply {
            setRules(RuleController.parseRules(context, R.xml.webcask_split_config))
        }
    }

    override fun dependencies(): List<Class<out Initializer<*>>> = emptyList()
}
