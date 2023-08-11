package com.tomcat.domain;

import lombok.Data;

import javax.persistence.*;

@Entity
@Data
@Table(name = "user_profile")
public class UserProfile {

  @Id
  @GeneratedValue(strategy = GenerationType.AUTO)
  private Long id;

  @OneToOne
  @JoinColumn(name="userId")
  private User user;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String motherTongue;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String languageDepth;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String communicationStyle;

  @Column(columnDefinition = "varchar(255) CHARACTER SET utf8mb4")
  private String targetLanguage;

  // constructor, getter and setter


}