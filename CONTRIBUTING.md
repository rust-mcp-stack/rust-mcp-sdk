# **Contributing to rust-mcp-sdk**

🎉 Thank you for your interest in improving **rust-mcp-sdk**! Every contribution, big or small, is valuable and appreciated.

## **Code of Conduct**

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please be respectful and inclusive when contributing.

## **How to Contribute**

### Participating in Tests, Documentation, and Examples

We highly encourage contributors to improve test coverage, enhance documentation, and introduce new examples to ensure the reliability and usability of the project. If you notice untested code paths, missing documentation, or areas where examples could help, consider adding tests, clarifying explanations, or providing real-world usage examples. Every improvement helps make the project more robust, well-documented, and accessible to others!

### Participating in Issues

You can contribute in three key ways:

1. **Report Issues** – If you find a bug or have an idea, open an issue for discussion.
2. **Help Triage** – Provide details, test cases, or suggestions to clarify issues.
3. **Resolve Issues** – Investigate problems and submit fixes via Pull Requests (PRs).

Anyone can participate at any stage, whether it's discussing, triaging, or reviewing PRs.

### **Filing a Bug Report**

When reporting a bug, use the provided issue template and fill in as many details as possible. Don’t worry if you can’t answer everything—just provide what you can.

### **Fixing Issues**

Most issues are resolved through a Pull Request. PRs go through a review process to ensure quality and correctness.

## **Pull Requests (PRs)**

We welcome PRs! Before submitting, please:

1. **Discuss major changes** – Open an issue before adding a new feature and opening a PR.
2. **Create a feature branch** – Fork the repo and branch from `main`.
3. **Write tests** – If your change affects functionality, add relevant tests.
4. **Update documentation** – If you modify APIs, update the docs.
5. **Run tests** – Make sure all tests succeed by running:

```sh
cargo make test
```

### **Commit Best Practices**

- **Relate PR changes to the issue** – Changes in a pull request (PR) should directly address the specific issue it’s tied to. Unrelated changes should be split into separate issues and PRs to maintain focus and simplify review.
- **Logically separate commits** – Keep changes atomic and easy to review.
- **Maintain a bisect-able history** – Each commit should compile and pass all tests to enable easy debugging with `git bisect` in case of regression.

## License

By contributing to rust-mcp-sdk, you acknowledge and agree that your contributions will be licensed under the terms specified in the LICENSE file located in the root directory of this repository.

---
