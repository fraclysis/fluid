use comrak::{
    self, plugins::syntect::SyntectAdapter, ComrakOptions, ComrakPlugins, ComrakRenderOptions,
};

const MARKDOWN_OPTIONS: ComrakOptions = {
    ComrakOptions {
        extension: comrak::ComrakExtensionOptions {
            strikethrough: true,
            tagfilter: false,
            table: true,
            autolink: true,
            tasklist: true,
            superscript: true,
            header_ids: None,
            footnotes: true,
            description_lists: true,
            front_matter_delimiter: None,
        },
        parse: comrak::ComrakParseOptions {
            smart: false,
            default_info_string: None,
            relaxed_tasklist_matching: true,
        },
        render: ComrakRenderOptions {
            hardbreaks: true,
            github_pre_lang: true,
            width: 20,
            unsafe_: true,
            escape: false,
            list_style: comrak::ListStyleType::Dash,
            full_info_string: false,
            sourcepos: false,
        },
    }
};

static mut MARKDOWN_ADAPTER: Option<SyntectAdapter> = None;

fn get_markdown_adapter() -> &'static SyntectAdapter {
    unsafe {
        match &MARKDOWN_ADAPTER {
            Some(adapter) => adapter,
            None => {
                let adapter = SyntectAdapter::new("base16-ocean.light");
                MARKDOWN_ADAPTER = Some(adapter);

                match &MARKDOWN_ADAPTER {
                    Some(adapter) => adapter,
                    None => panic!("Expected to have an adapter."),
                }
            }
        }
    }
}

pub fn markdown(text: &str) -> String {
    let adapter = get_markdown_adapter();
    let options = &MARKDOWN_OPTIONS;

    let plugins = ComrakPlugins {
        render: comrak::ComrakRenderPlugins {
            codefence_syntax_highlighter: Some(adapter),
            heading_adapter: None,
        },
    };

    comrak::markdown_to_html_with_plugins(text, options, &plugins)
}
