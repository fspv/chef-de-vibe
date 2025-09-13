import type { AnyTodoItem, OfficialTodoItem } from '../types/claude-messages';
import { isExtendedTodoItem } from '../types/claude-messages';

interface TodoListProps {
  todos: AnyTodoItem[];
}

function getStatusIcon(status: string): string {
  switch (status) {
    case 'completed':
      return '✅';
    case 'in_progress':
      return '🔄';
    case 'pending':
    default:
      return '⏳';
  }
}

function getPriorityClass(priority: string): string {
  switch (priority) {
    case 'high':
      return 'priority-high';
    case 'medium':
      return 'priority-medium';
    case 'low':
      return 'priority-low';
    default:
      return 'priority-medium';
  }
}

function TodoItemComponent({ todo }: { todo: AnyTodoItem }) {
  const statusIcon = getStatusIcon(todo.status);
  
  if (isExtendedTodoItem(todo)) {
    const priorityClass = getPriorityClass(todo.priority);
    
    return (
      <div className={`todo-item ${todo.status} ${priorityClass}`}>
        <span className="todo-status">{statusIcon}</span>
        <span className="todo-id">#{todo.id}</span>
        <span className={`todo-priority ${priorityClass}`}>
          {todo.priority.toUpperCase()}
        </span>
        <span className="todo-content">{todo.content}</span>
      </div>
    );
  } else {
    // Official format with activeForm
    return (
      <div className={`todo-item ${todo.status}`}>
        <span className="todo-status">{statusIcon}</span>
        <span className="todo-form">{(todo as OfficialTodoItem).activeForm}</span>
        <span className="todo-content">{todo.content}</span>
      </div>
    );
  }
}

export function TodoList({ todos }: TodoListProps) {
  if (!todos || todos.length === 0) {
    return null;
  }

  const pendingCount = todos.filter(todo => todo.status === 'pending').length;
  const inProgressCount = todos.filter(todo => todo.status === 'in_progress').length;
  const completedCount = todos.filter(todo => todo.status === 'completed').length;

  return (
    <div className="todo-list">
      <div className="todo-list-header">
        <h4>📋 Todo List</h4>
        <div className="todo-stats">
          <span className="stat pending">{pendingCount} pending</span>
          <span className="stat in-progress">{inProgressCount} in progress</span>
          <span className="stat completed">{completedCount} completed</span>
        </div>
      </div>
      
      <div className="todo-items">
        {todos.map((todo, index) => (
          <TodoItemComponent 
            key={isExtendedTodoItem(todo) ? todo.id : `todo-${index}`} 
            todo={todo} 
          />
        ))}
      </div>
    </div>
  );
}