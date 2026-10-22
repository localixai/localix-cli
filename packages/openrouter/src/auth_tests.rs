use super::*;

#[test]
fn the_challenge_is_the_verifiers_sha256() {
    let (verifier, challenge) = pkce();
    assert_eq!(verifier.len(), 43);
    assert_eq!(
        challenge,
        URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
    );
    assert_ne!(pkce().0, verifier);
}

#[test]
fn reads_the_code_from_a_redirect_or_a_bare_paste() {
    assert_eq!(
        code_from_paste("http://127.0.0.1:5/oauth/callback/x?code=abc&y=1"),
        Some("abc".into())
    );
    assert_eq!(
        code_from_paste("/oauth/callback/x?code=abc#frag"),
        Some("abc".into())
    );
    assert_eq!(code_from_paste("  abc \n"), Some("abc".into()));
    assert_eq!(code_from_paste("http://x/?error=denied"), None);
    assert_eq!(code_from_paste(""), None);
}
