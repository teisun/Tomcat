package com.tomcat.controller.response;

import lombok.Data;

import java.util.List;


@Data
public class ChatAssistantDataResp {
    private String topic;
    private String assistant_sentence;
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
        private String better_answer;
        private List<String> points;

        // 构造函数、getter 和 setter 方法
    }
}
