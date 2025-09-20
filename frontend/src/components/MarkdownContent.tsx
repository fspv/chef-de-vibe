import Markdown from 'react-markdown';
import type { Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { Prism as SyntaxHighlighter } from 'react-syntax-highlighter';
import { oneDark } from 'react-syntax-highlighter/dist/esm/styles/prism';
import 'github-markdown-css/github-markdown-light.css';

interface MarkdownContentProps {
  content: string;
  className?: string;
}

interface CodeProps {
  inline?: boolean;
  className?: string;
  children?: React.ReactNode;
}

export function MarkdownContent({ content, className }: MarkdownContentProps) {
  const components: Components = {
    code(props) {
      const { inline, className, children } = props as CodeProps;
          const match = /language-(\w+)/.exec(className || '');
          const language = match ? match[1] : '';
          
          if (!inline && language) {
            return (
              <SyntaxHighlighter
                style={oneDark}
                language={language}
                PreTag="div"
              >
                {String(children).replace(/\n$/, '')}
              </SyntaxHighlighter>
            );
          }
          
          return (
            <code className={className}>
              {children}
            </code>
          );
        },
        // Style links to open in new tab
        a({ children, href, ...props }) {
          return (
            <a href={href} target="_blank" rel="noopener noreferrer" {...props}>
              {children}
            </a>
          );
        },
        // Style tables
        table({ children, ...props }) {
          return (
            <div className="table-wrapper">
              <table {...props}>{children}</table>
            </div>
          );
    }
  };

  return (
    <div className={`markdown-body ${className || ''}`}>
      <Markdown
        remarkPlugins={[remarkGfm]}
        components={components}
      >
        {content}
      </Markdown>
    </div>
  );
}