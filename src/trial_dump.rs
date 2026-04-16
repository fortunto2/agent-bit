//! Trial debug data dump — pipeline.txt, inbox files, tree, contacts/accounts, agents.md.

/// Dump trial debug data to disk.
pub(crate) fn dump_trial_data(
    dump_dir: &str, tree_out: &str, agents_md: &str, crm_schema: &str,
    ready: &crate::pipeline::Ready, model: &str, intent_confidence: f32,
) {
    let _ = std::fs::create_dir_all(dump_dir);
    let _ = std::fs::write(format!("{dump_dir}/tree.txt"), tree_out);
    if !agents_md.is_empty() { let _ = std::fs::write(format!("{dump_dir}/agents.md"), agents_md); }
    if !crm_schema.is_empty() { let _ = std::fs::write(format!("{dump_dir}/crm_schema.txt"), crm_schema); }
    let contacts = ready.crm_graph.contacts_summary();
    if !contacts.is_empty() { let _ = std::fs::write(format!("{dump_dir}/contacts.txt"), &contacts); }
    let accounts = ready.crm_graph.accounts_summary();
    if !accounts.is_empty() { let _ = std::fs::write(format!("{dump_dir}/accounts.txt"), &accounts); }
    for (i, f) in ready.inbox_files.iter().enumerate() {
        let sender = f.security.sender.as_ref().map(|s| format!("{}", s.trust)).unwrap_or_default();
        let _ = std::fs::write(
            format!("{dump_dir}/inbox_{i:02}_{}.txt", f.path.replace('/', "_")),
            format!("[{} ({:.2}) | sender: {sender} | {}]\n\n{}", f.security.ml_label, f.security.ml_conf, f.security.recommendation, f.content),
        );
    }
    let per_inbox: Vec<String> = ready.inbox_files.iter().enumerate().map(|(i, f)| {
        let sender = f.security.sender.as_ref().map(|s| format!("{}", s.trust)).unwrap_or_else(|| "?".into());
        format!("  [{i}] {} ({:.2}) sender={sender} {}", f.security.ml_label, f.security.ml_conf, f.path)
    }).collect();
    let _ = std::fs::write(format!("{dump_dir}/pipeline.txt"), format!(
        "model: {model}\ninstruction: {}\nintent: {} ({intent_confidence:.2})\nlabel: {}\ninbox_files: {}\ncrm_nodes: {}\n\nper_inbox:\n{}\n",
        ready.instruction, ready.intent, ready.instruction_label, ready.inbox_files.len(), ready.crm_graph.node_count(), per_inbox.join("\n"),
    ));
    eprintln!("  📁 Trial data dumped to {dump_dir}");
}
