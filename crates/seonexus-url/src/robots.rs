//! robots.txt parsing and matching (RFC 9309, as implemented by Google).
//!
//! - Groups start with one or more `user-agent` lines; the rules that follow apply to all of
//!   them. Groups naming the same agent are merged.
//! - A crawler obeys the group whose agent token is the most specific match of its own tokens
//!   (case-insensitive; `googlebot` also covers `googlebot-image` when no closer group
//!   exists), else the `*` group, else nothing is restricted.
//! - Among matching rules the longest pattern wins; on equal length `allow` wins. `*` matches
//!   any sequence and a trailing `$` anchors the end. `/robots.txt` is always allowed.

/// One `allow` / `disallow` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RobotsRule {
    /// `true` for `allow`.
    pub allow: bool,
    /// Path pattern as written.
    pub pattern: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Group {
    agents: Vec<String>,
    rules: Vec<RobotsRule>,
}

/// A parsed robots.txt.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Robots {
    groups: Vec<Group>,
    /// `sitemap` URLs.
    pub sitemaps: Vec<String>,
}

/// The decision for one URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RobotsDecision<'a> {
    /// Whether crawling is allowed.
    pub allowed: bool,
    /// The deciding rule, if any.
    pub rule: Option<&'a RobotsRule>,
}

/// The rules a crawler obeys (see [`Robots::group_for`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentRules<'a> {
    /// Agent token of the group used (`*` for the default group); `None` without a group.
    pub group: Option<&'a str>,
    rules: Option<&'a [RobotsRule]>,
}

impl Robots {
    /// Parses robots.txt text; unknown lines are ignored, as crawlers do.
    pub fn parse(text: &str) -> Self {
        let mut robots = Self::default();
        let mut current: Option<Group> = None;
        let mut last_was_agent = false;
        for line in text.lines() {
            let line = line.split('#').next().unwrap_or("").trim();
            let Some((key, value)) = line.split_once(':') else {
                continue;
            };
            let value = value.trim();
            match key.trim().to_ascii_lowercase().as_str() {
                "user-agent" | "useragent" | "user agent" => {
                    if !last_was_agent {
                        if let Some(group) = current.take() {
                            robots.push_group(&group);
                        }
                        current = Some(Group::default());
                    }
                    if let Some(group) = current.as_mut() {
                        group.agents.push(value.to_ascii_lowercase());
                    }
                    last_was_agent = true;
                }
                rule @ ("allow" | "disallow") => {
                    last_was_agent = false;
                    if let Some(group) = current.as_mut()
                        && !value.is_empty()
                    {
                        group.rules.push(RobotsRule {
                            allow: rule == "allow",
                            pattern: value.to_owned(),
                        });
                    }
                }
                "sitemap" => {
                    if !value.is_empty() {
                        robots.sitemaps.push(value.to_owned());
                    }
                }
                _ => last_was_agent = false,
            }
        }
        if let Some(group) = current {
            robots.push_group(&group);
        }
        robots
    }

    fn push_group(&mut self, group: &Group) {
        // Merge with groups naming the same agents.
        for agent in &group.agents {
            if let Some(existing) = self.groups.iter_mut().find(|g| g.agents == [agent.clone()]) {
                existing.rules.extend(group.rules.iter().cloned());
            } else {
                self.groups.push(Group {
                    agents: vec![agent.clone()],
                    rules: group.rules.clone(),
                });
            }
        }
    }

    /// The rules for a crawler identified by `tokens` (e.g. `["googlebot-image",
    /// "googlebot"]`). `use_default` is `false` for crawlers that ignore the `*` group
    /// (Google's AdsBot and Mediapartners).
    pub fn group_for(&self, tokens: &[&str], use_default: bool) -> AgentRules<'_> {
        let mut best: Option<(&Group, usize)> = None;
        for group in &self.groups {
            for agent in &group.agents {
                if agent == "*" {
                    continue;
                }
                let matches = tokens.iter().any(|token| {
                    let token = token.to_ascii_lowercase();
                    token == *agent
                        || token
                            .strip_prefix(agent.as_str())
                            .is_some_and(|rest| rest.starts_with('-'))
                });
                if matches && best.is_none_or(|(_, len)| agent.len() > len) {
                    best = Some((group, agent.len()));
                }
            }
        }
        if let Some((group, _)) = best {
            return AgentRules {
                group: group.agents.first().map(String::as_str),
                rules: Some(&group.rules),
            };
        }
        let default = use_default
            .then(|| {
                self.groups
                    .iter()
                    .find(|g| g.agents.iter().any(|a| a == "*"))
            })
            .flatten();
        AgentRules {
            group: default.map(|_| "*"),
            rules: default.map(|g| g.rules.as_slice()),
        }
    }

    /// Whether the robots.txt names any group.
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }
}

impl<'a> AgentRules<'a> {
    /// Decides `target` (path and query, e.g. `/a?b=1`).
    pub fn decide(&self, target: &str) -> RobotsDecision<'a> {
        let allowed = RobotsDecision {
            allowed: true,
            rule: None,
        };
        let Some(rules) = self.rules else {
            return allowed;
        };
        if target == "/robots.txt" {
            return allowed;
        }
        let mut best: Option<&RobotsRule> = None;
        for rule in rules {
            if !pattern_matches(&rule.pattern, target) {
                continue;
            }
            let better = best.is_none_or(|current| {
                rule.pattern.len() > current.pattern.len()
                    || (rule.pattern.len() == current.pattern.len() && rule.allow && !current.allow)
            });
            if better {
                best = Some(rule);
            }
        }
        RobotsDecision {
            allowed: best.is_none_or(|rule| rule.allow),
            rule: best,
        }
    }

    /// Whether the whole site is off limits (`/` disallowed).
    pub fn blocks_everything(&self) -> bool {
        !self.decide("/").allowed
    }
}

/// Matches a robots pattern (prefix match, `*` wildcard, `$` end anchor) against a target.
fn pattern_matches(pattern: &str, target: &str) -> bool {
    let (pattern, anchored) = match pattern.strip_suffix('$') {
        Some(rest) => (rest, true),
        None => (pattern, false),
    };
    let pattern = pattern.as_bytes();
    let target = target.as_bytes();
    // Iterative wildcard matching with backtracking to the last `*`.
    let (mut p, mut t) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    loop {
        if p == pattern.len() {
            if !anchored || t == target.len() {
                return true;
            }
        } else if pattern[p] == b'*' {
            star = Some((p, t));
            p += 1;
            continue;
        } else if t < target.len() && pattern[p] == target[t] {
            p += 1;
            t += 1;
            continue;
        }
        match star {
            Some((star_p, star_t)) if star_t < target.len() => {
                star = Some((star_p, star_t + 1));
                p = star_p + 1;
                t = star_t + 1;
            }
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROBOTS: &str = "\
# Example
User-agent: *
Disallow: /admin/
Disallow: /*?sessionid=
Allow: /admin/public/
Disallow: /*.pdf$

User-agent: GPTBot
User-agent: CCBot
Disallow: /

User-agent: Googlebot
Disallow: /private
Allow: /private/ok

User-agent: googlebot-image
Disallow: /photos/

Sitemap: https://example.com/sitemap.xml
";

    fn decide(robots: &Robots, tokens: &[&str], target: &str) -> bool {
        robots.group_for(tokens, true).decide(target).allowed
    }

    #[test]
    fn picks_the_most_specific_group() {
        let robots = Robots::parse(ROBOTS);
        assert_eq!(robots.sitemaps, ["https://example.com/sitemap.xml"]);
        assert_eq!(robots.group_for(&["GPTBot"], true).group, Some("gptbot"));
        assert_eq!(robots.group_for(&["ccbot"], true).group, Some("ccbot"));
        assert_eq!(
            robots
                .group_for(&["googlebot-image", "googlebot"], true)
                .group,
            Some("googlebot-image")
        );
        assert_eq!(
            robots.group_for(&["googlebot-news"], true).group,
            Some("googlebot")
        );
        assert_eq!(robots.group_for(&["bingbot"], true).group, Some("*"));
        assert_eq!(robots.group_for(&["adsbot-google"], false).group, None);
        assert!(robots.group_for(&["gptbot"], true).blocks_everything());
        assert!(!robots.group_for(&["bingbot"], true).blocks_everything());
    }

    #[test]
    fn longest_match_wins_and_allow_breaks_ties() {
        let robots = Robots::parse(ROBOTS);
        let bing = ["bingbot"];
        assert!(!decide(&robots, &bing, "/admin/x"));
        assert!(decide(&robots, &bing, "/admin/public/page"));
        assert!(!decide(&robots, &bing, "/shop?sessionid=1"));
        assert!(!decide(&robots, &bing, "/files/a.pdf"));
        assert!(decide(&robots, &bing, "/files/a.pdf?x=1"));
        assert!(decide(&robots, &bing, "/"));
        let google = ["googlebot"];
        // The Googlebot group replaces `*`, so /admin/ is allowed for Googlebot.
        assert!(decide(&robots, &google, "/admin/x"));
        assert!(!decide(&robots, &google, "/private/x"));
        assert!(decide(&robots, &google, "/private/ok"));
        assert!(decide(&robots, &["gptbot"], "/robots.txt"));
        let tie = Robots::parse("User-agent: *\nDisallow: /page\nAllow: /page\n");
        assert!(decide(&tie, &bing, "/page"));
        let rule = robots
            .group_for(&bing, true)
            .decide("/admin/x")
            .rule
            .unwrap();
        assert_eq!((rule.allow, rule.pattern.as_str()), (false, "/admin/"));
    }

    #[test]
    fn wildcards_and_anchors() {
        assert!(pattern_matches("/", "/anything"));
        assert!(pattern_matches("/*/amp", "/blog/post/amp"));
        assert!(pattern_matches("/a*b*c", "/aXXbYYc"));
        assert!(!pattern_matches("/a*b*c", "/aXXcYYb"));
        assert!(pattern_matches("/x$", "/x"));
        assert!(!pattern_matches("/x$", "/xy"));
        assert!(pattern_matches("*.php$", "/index.php"));
        assert!(!pattern_matches("/private", "/priv"));
    }

    #[test]
    fn tolerates_messy_files() {
        let robots = Robots::parse(
            "Disallow: /orphan\nuser-agent:*\ndisallow:\nDISALLOW: /x # note\nnonsense line\n",
        );
        assert!(decide(&robots, &["any"], "/orphan"));
        assert!(!decide(&robots, &["any"], "/x/y"));
        assert!(Robots::parse("").is_empty());
        assert!(decide(&Robots::parse(""), &["any"], "/x"));
    }
}
