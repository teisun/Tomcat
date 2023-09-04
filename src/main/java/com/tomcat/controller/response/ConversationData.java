package com.tomcat.controller.response;

import lombok.Data;

import java.util.List;


@Data
public class ConversationData {
    private String sentence;
    private String translate;
    private List<String> tips;
    private Mission mission;
    private List<Suggestion> suggestions;

    // 构造函数、getter 和 setter 方法

    @Data
    public static class Mission {
        private String situation;
        private List<Action> actions;

        // 构造函数、getter 和 setter 方法

        @Data
        public static class Action {
            private int status;
            private String text;

            // 构造函数、getter 和 setter 方法
        }
    }

    @Data
    public static class Suggestion {
        private String wrong;
        private String right;
        private String feedback;

        // 构造函数、getter 和 setter 方法
    }
}
