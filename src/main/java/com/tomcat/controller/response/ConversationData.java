package com.tomcat.controller.response;

import lombok.Data;

import java.util.List;


@Data
public class ConversationData {
    private String sentence;
    private String translate;
    private List<String> tips;
    private List<Mission> missions;
    private Suggestion suggestion;

    // 构造函数、getter 和 setter 方法

    @Data
    public static class Mission {

        private int status;
        private String text;

            // 构造函数、getter 和 setter 方法
    }

    @Data
    public static class Suggestion {
        private String correct_answer;
        private List<String> points;

        // 构造函数、getter 和 setter 方法
    }
}
