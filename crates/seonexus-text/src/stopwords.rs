//! Stop words: function words that carry no topic.

use std::collections::HashSet;
use std::sync::LazyLock;

use crate::Language;

const TURKISH: &str = "
acaba ama ancak artık aslında az bana bazen bazı bazıları belki ben benden beni benim beri
bile bir birçok biri birkaç birşey biz bizden bize bizi bizim bu buna bunda bundan bunlar
bunları bunların bunu bunun burada böyle böylece çok çünkü da daha dahi de defa değil diğer
diye dolayı en fakat gibi göre hala hangi hatta hem hep hepsi her herhangi herkes hiç hiçbir
için ile ise işte kadar kendi kendine ki kim kimse mi mı mu mü nasıl ne neden nerde nerede
nereye niçin niye o olan olarak oldu olduğu olduğunu olmak olmaz olsa olur on ona ondan onlar
onlardan onları onların onu onun orada öyle oysa pek rağmen sadece sanki şey siz sizden size
sizi sizin şu şuna şunda şundan şunlar şunu şunun tabii tüm üzere var vardı ve veya ya yani
yapılan yine yoksa zaten değildir olduğunu olduğu sonra önce ayrıca yer yerine kez tarafından
arasında içinde üzerinde olan olanlar olabilir olacak olarak edilen eden etmek etti yapmak
yaptı bunlar şimdi bugün dolayısıyla genellikle özellikle çoğu hemen göre böylelikle bunlara
";

const ENGLISH: &str = "
a about above after again against all am an and any are aren't as at be because been before
being below between both but by can can't cannot could couldn't did didn't do does doesn't
doing don't down during each few for from further had hadn't has hasn't have haven't having he
her here hers herself him himself his how i if in into is isn't it it's its itself just let me
more most mustn't my myself no nor not now of off on once only or other ought our ours
ourselves out over own same shan't she should shouldn't so some such than that the their
theirs them themselves then there these they this those through to too under until up very
was wasn't we were weren't what when where which while who whom why will with won't would
wouldn't you your yours yourself yourselves also may might must shall s t can will get got
one two us like via per
";

const GERMAN: &str = "
aber alle allem allen aller alles als also am an ander andere anderem anderen anderer anderes
auch auf aus bei bin bis bist da damit dann das dass dein deine dem den der des dich die dies
diese diesem diesen dieser dieses dir doch dort du durch ein eine einem einen einer eines er
es etwas euch euer für gegen hab habe haben hat hatte hier hin hinter ich ihm ihn ihnen ihr
ihre im in indem ins ist jede jedem jeden jeder jedes jetzt kann kein keine können man mein
meine mich mir mit muss nach nicht nichts noch nun nur ob oder ohne sehr sein seine sich sie
sind so solche soll sondern sonst über um und uns unser unter vom von vor war waren warum was
weil welche wenn wer werden wie wieder will wir wird wo zu zum zur zwischen
";

const RUSSIAN: &str = "
а без более бы был была были было быть в вам вас весь во вот все всего всех вы где да даже
для до его ее ей ему если есть еще же за здесь и из или им их к как ко когда кто ли либо мне
может мы на над надо наш не него нее нет ни них но ну о об однако он она они оно от очень по
под при с со так также такой там те тем то того тоже той только том ты у уже хотя чего чей
чем что чтобы чье чья эта эти это я
";

fn set(words: &'static str) -> HashSet<&'static str> {
    words.split_whitespace().collect()
}

static SETS: LazyLock<[HashSet<&'static str>; 4]> =
    LazyLock::new(|| [set(TURKISH), set(ENGLISH), set(GERMAN), set(RUSSIAN)]);

/// Whether a lowercased word is a stop word of the language.
pub fn is_stopword(word: &str, language: Language) -> bool {
    let index = match language {
        Language::Turkish => 0,
        Language::English => 1,
        Language::German => 2,
        Language::Russian => 3,
        Language::Other => return false,
    };
    SETS[index].contains(word)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_common_function_words() {
        assert!(is_stopword("ve", Language::Turkish));
        assert!(is_stopword("için", Language::Turkish));
        assert!(!is_stopword("kahve", Language::Turkish));
        assert!(is_stopword("the", Language::English));
        assert!(is_stopword("und", Language::German));
        assert!(is_stopword("что", Language::Russian));
        assert!(!is_stopword("the", Language::Other));
    }
}
